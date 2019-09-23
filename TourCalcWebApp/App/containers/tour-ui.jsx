import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

export default class TourUI extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isAuthLoaded: false,
        }
    }

    componentDidMount() {
        AppState.checkWhoAmI(this);
    }
    render() {
        if (!this.state.isAuthLoaded) {
            return (<div>Checking Who you are...</div>)
        } else {
            if (!this.state.authData.isMaster
                && this.state.authData.tourIds.indexOf(this.props.tourid) == -1) { // no such tour for credentials
                return (<div><pre>{JSON.stringify(this.state.authData, null, 2)}</pre><TourEnterAccessCode app={this} /></div>)
            } else {
                return (<TourTable tourid={this.props.tourid} />)
            }

        }
        
    }
}

class TourTable extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isTourLoaded: false,
            tour: null
        }
    }

    componentDidMount() {
        AppState.loadTour(this, this.props.tourid);
    }

    render() {
        if (!this.state.isTourLoaded) {
            return <div>Tour {this.props.tourid} loading...</div>
        } else {
            return <div><pre>Tour: {JSON.stringify(this.state.tour, null, 2)}}</pre></div>
        }
    }

}

class TourEnterAccessCode extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isAuthLoaded: false
        }
    }

    code = 'wrong_code'

    render() {
        return (
            <div>
                <form onSubmit={(event) => {
                    event.preventDefault();
                    AppState.login(this.props.app, 'code', this.code)
                }}>
                    <p>access code:</p>
                    <input
                        type='text'
                        onChange={(e) => this.code = event.target.value}
                    />
                    <input
                        type='submit'
                    />
                </form>
            </div>
            )
    }

}