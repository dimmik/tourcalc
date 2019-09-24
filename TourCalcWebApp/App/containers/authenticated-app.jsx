import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import Cookies from 'js-cookie';
import { BrowserRouter as Router, Route, Switch, Link } from 'react-router-dom';
import TourAdd from './tour-add.jsx'

export default class AuthenticatedApp extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isToursLoaded: false,
            tours: null
        }
    }
    componentDidMount() {
        AppState.loadTours(this);
    }

    render() {
        if (!this.state.isToursLoaded) {
            return <div>Loading Tours ...</div>
        } else {
            return (
                <div>
                    Tours {this.props.authData.type === 'Master' ? <TourAdd buttonText="Add" actionButtonText="AddTour" app={this} open={false}/> : <span/>}
                    <ol>
                        {this.state.tours.map( t =>
                            (
                                <li key={t.id}><Link to={'/tour/' + t.id}>{t.name}</Link>
                                    {this.props.authData.type === 'Master' ? (
                                        <button onClick={() => {
                                            if (window.confirm('Sure to delete tour ' + t.name + ' (id: ' + t.id + ')?')) {
                                                AppState.deleteTour(this, t.id)
                                                    .then(() => { AppState.loadTours(this); })
                                            }
                                        }}>Del</button>) : <span />
                                    }
                                    </li>
                            )
                        )
                        }
                    </ol>
                </div>
                )
        }
    }

};