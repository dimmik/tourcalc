import React from 'react';
import ReactDOM from 'react-dom';
import { BrowserRouter as Router, Route, Switch } from 'react-router-dom';
import Header from './header/header.jsx';
import TourPersons from './tour/persons.jsx';
import TourSpendings from './tour/spendings.jsx';
import TourChoose from './tour/choose.jsx';
import Auth from './auth/auth.jsx';

export default class App extends React.Component {
    constructor(props) {
        super(props);
        this.chooseTour = this.chooseTour.bind(this);
        this.state = {
            isTourChosen: false,
            chosenTour: null
        };
    }
    chooseTour(tour) {
        this.setState({
            isTourChosen: true,
            chosenTour: tour
        })
    }
    render() {
        return (
            <Router>
                <div>
                    <Header tour={this.state.chosenTour}/>
                    <main>
                        <Switch>
                            <Route path="/spendings" render={(props) => <TourSpendings tourid={this.state.isTourChosen ? this.state.chosenTour.id : null} />} />
                            <Route path="/auth" component={Auth} />
                            <Route path="/persons" render={(props) => <TourPersons tourid={this.state.isTourChosen ? this.state.chosenTour.id : null} />} />
                            <Route path="/"
                                render={(props) => <TourChoose chosenTour={this.state.chosenTour} chooseTourAction={this.chooseTour} />} />
                        </Switch>
                    </main>
                </div>
            </Router>
        );
    }
};
ReactDOM.render(
    <App />,
    document.getElementById("content")
);